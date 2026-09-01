//! Long-lived automation built on tracked Work and bounded Runs.

pub mod project;
mod runner;
mod store;
pub mod task;
pub mod wave;

pub use runner::run_work;
