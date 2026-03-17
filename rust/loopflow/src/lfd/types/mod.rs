//! Native domain types for lfd.
//!
//! These replace the proto-generated types, giving us clean Rust types
//! without gRPC/proto dependencies.

mod agent;
mod attention;
mod chat_memory;
mod chat_message;
mod event;
mod repo;
mod summary;
mod terminal_session;
mod trigger;
mod wave;

pub use agent::{AgentRun, AgentStatus};
pub use attention::{AttentionItem, AttentionKind, AttentionStatus};
pub use chat_memory::ChatMemoryBlock;
pub use chat_message::ChatMessage;
pub use event::Event;
pub use repo::{Repo, RepoEdge, RepoId};
pub use summary::Summary;
pub use terminal_session::{TerminalSession, TerminalSessionStatus};
pub use trigger::{
    ActivationLog, ActivationOutcome, PendingActivation, Signal, Trigger, CI_FIX_FLOW,
};
pub use wave::{
    LivePrState, LivePullRequestState, PullRequest, QueueBlock, QueueBlockReason, QueueMergeEvent,
    Wave, WaveMode, WaveRun, WaveRunSnapshot, WaveRunStackStatus, WaveRunStatus, WaveStatus,
};
