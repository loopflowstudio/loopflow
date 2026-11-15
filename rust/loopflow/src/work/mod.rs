//! Durable tracked Work, independent of any autonomous controller.

pub use crate::store::work::edit;
pub mod link;
pub mod link_receiver;
pub use crate::store::work::project;
pub use crate::store::work::selection;
pub use crate::store::work::task;
pub mod wave;
