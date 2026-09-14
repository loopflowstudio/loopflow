//! Long-lived automation built on tracked Work and bounded Runs.

pub mod authority;
pub mod project;
mod runner;
pub(crate) mod startup;
mod store;
pub mod task;
pub mod wave;

pub use runner::run_work;
#[doc(hidden)]
pub use startup::WorkStartupAttempt;
