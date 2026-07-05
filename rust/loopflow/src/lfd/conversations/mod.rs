//! Compatibility shims for the old `lfd::conversations` paths.
//!
//! The owning modules are now [`crate::conversation`] for shared wire
//! vocabulary and [`crate::harness`] for vendor drivers.

pub mod harness {
    pub use crate::harness::*;
}
pub mod opencode_runtime;
pub mod turns;
pub mod types;
