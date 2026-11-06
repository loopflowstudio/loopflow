//! A detached terminal owns an execution, including its capture and native client.
//! Viewers hold revocable input tokens, never provider attachment capabilities.

mod input;
#[cfg(test)]
mod input_retry_tests;
mod lifetime;
#[cfg(test)]
mod lifetime_tests;
mod native;
mod output;
pub(crate) mod remote;
