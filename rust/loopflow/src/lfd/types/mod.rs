//! Native domain types for lfd.
//!
//! These replace the proto-generated types, giving us clean Rust types
//! without gRPC/proto dependencies.

mod agent;
mod chat_memory;
mod event;
mod stimulus;
mod summary;
mod wave;

pub use agent::{Agent, AgentStatus};
pub use chat_memory::ChatMemoryBlock;
pub use event::Event;
pub use stimulus::{PendingActivation, Stimulus, StimulusKind};
pub use summary::Summary;
pub use wave::{
    LivePrState, LivePullRequestState, PullRequest, SidecarKind, Wave, WaveRun, WaveRunKind,
    WaveRunSnapshot, WaveRunStackStatus, WaveRunStatus, WaveStatus,
};
