//! Native domain types for lfd.
//!
//! These replace the proto-generated types, giving us clean Rust types
//! without gRPC/proto dependencies.

mod agent;
mod chat_memory;
mod chat_message;
mod chord;
mod event;
mod stimulus;
mod summary;
mod wave;

pub use agent::{AgentRun, AgentStatus};
pub use chat_memory::ChatMemoryBlock;
pub use chat_message::ChatMessage;
pub use chord::Chord;
pub use event::Event;
pub use stimulus::{
    ActivationLog, ActivationOutcome, ActivationSource, PendingActivation, Stimulus, StimulusKind,
};
pub use summary::Summary;
pub use wave::{
    CiFixKind, LivePrState, LivePullRequestState, PullRequest, QueueBlock, QueueBlockReason,
    QueueMergeEvent, Wave, WaveRun, WaveRunKind, WaveRunSnapshot, WaveRunStackStatus,
    WaveRunStatus, WaveStatus,
};
