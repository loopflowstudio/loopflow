//! Native domain types for lfd.
//!
//! These replace the proto-generated types, giving us clean Rust types
//! without gRPC/proto dependencies.

mod agent;
mod event;
mod stimulus;
mod wave;

pub use agent::{Agent, AgentStatus};
pub use event::Event;
pub use stimulus::{PendingActivation, Stimulus, StimulusKind};
pub use wave::{PullRequest, Wave, WaveRun, WaveRunSnapshot, WaveRunStatus, WaveStatus};
