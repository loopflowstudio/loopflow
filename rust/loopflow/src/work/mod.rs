//! Durable tracked Work, independent of any autonomous controller.

pub use loopflow_storage::work::edit;
pub mod link;
pub mod link_receiver;
pub use loopflow_storage::work::project;
pub use loopflow_storage::work::selection;
pub use loopflow_storage::work::task;
pub mod wave;
