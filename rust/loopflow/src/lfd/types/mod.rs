//! Native domain types for lfd.
//!
//! These replace the proto-generated types, giving us clean Rust types
//! without gRPC/proto dependencies.

mod agent;
mod chat_memory;
mod chat_message;
mod event;
mod stimulus;
mod summary;
mod wave;

pub use agent::{Agent, AgentStatus};
pub use chat_memory::ChatMemoryBlock;
pub use chat_message::ChatMessage;
pub use event::Event;
pub use stimulus::{PendingActivation, Stimulus, StimulusKind};
pub use summary::Summary;
pub use wave::{
    LivePrState, LivePullRequestState, PullRequest, QueueBlock, QueueBlockReason, QueueMergeEvent,
    SidecarKind, Wave, WaveData, WaveRun, WaveRunKind, WaveRunSnapshot, WaveRunStackStatus,
    WaveRunStatus, WaveStatus,
};
