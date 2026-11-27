//! Machine rental records and provider operations.

pub mod github;
pub mod sandbox;
pub use loopflow_storage::rental::*;

pub mod recovery;

/// A time as a person here reads it on a clock.
pub fn clock(at: time::OffsetDateTime) -> String {
    chrono::DateTime::from_timestamp(at.unix_timestamp(), 0)
        .map(|at| at.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_else(|| at.to_string())
}
